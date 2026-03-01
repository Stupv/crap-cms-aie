--- Queued job: send email notification + webhook for new inquiries.
local M = {}

---@param context table { data: { inquiry_id: string, name: string, email: string, service: string? }, job: table }
function M.run(context)
	local inquiry_id = context.data and context.data.inquiry_id
	if not inquiry_id then
		crap.log.error("process_inquiry: missing inquiry_id")
		return
	end

	local inquiry = crap.collections.find_by_id("inquiries", inquiry_id)
	if not inquiry then
		crap.log.warn("process_inquiry: inquiry not found: " .. inquiry_id)
		return
	end

	-- Send email notification
	crap.email.send({
		to = "hello@meridian.studio",
		subject = string.format("New inquiry from %s", inquiry.name or "Unknown"),
		html = string.format(
			"<h2>New Inquiry</h2>"
				.. "<p><strong>From:</strong> %s (%s)</p>"
				.. "<p><strong>Company:</strong> %s</p>"
				.. "<p><strong>Budget:</strong> %s</p>"
				.. "<p><strong>Message:</strong></p><p>%s</p>",
			inquiry.name or "",
			inquiry.email or "",
			inquiry.company or "N/A",
			inquiry.budget_range or "Not specified",
			inquiry.message or ""
		),
	})

	-- Send webhook notification
	local webhook_url = crap.config.get("meridian.inquiry_webhook_url")
	if webhook_url then
		local ok, err = pcall(function()
			crap.http.request({
				method = "POST",
				url = webhook_url,
				headers = {
					["Content-Type"] = "application/json",
				},
				body = crap.json.encode({
					event = "new_inquiry",
					inquiry_id = inquiry_id,
					name = inquiry.name,
					email = inquiry.email,
					company = inquiry.company,
					budget_range = inquiry.budget_range,
				}),
			})
		end)
		if not ok then
			crap.log.warn("process_inquiry: webhook failed: " .. tostring(err))
		end
	end

	-- Update status to contacted
	crap.collections.update("inquiries", inquiry_id, {
		status = "contacted",
	})

	crap.log.info("Processed inquiry: " .. inquiry_id)
end

crap.jobs.define("process_inquiry", {
	handler = "jobs.process_inquiry.run",
	queue = "notifications",
	retries = 3,
	timeout = 30,
	labels = { singular = "Process Inquiry" },
})

return M
