crap.log.info("Crap Studio initializing...")

-- Load plugins (runs after collections/*.lua are loaded)
require("plugins.seo").install({ exclude = { "pages", "inquiries" } })

-- ── Custom richtext nodes ────────────────────────────────────

-- Block-level: Call to Action button
crap.richtext.register_node("cta", {
	label = "Call to Action",
	inline = false,
	attrs = {
		{ name = "text", type = "text", label = "Button Text", required = true },
		{ name = "url", type = "text", label = "URL", required = true },
		{ name = "style", type = "select", label = "Style", options = {
			{ label = "Primary", value = "primary" },
			{ label = "Secondary", value = "secondary" },
			{ label = "Outline", value = "outline" },
		}},
	},
	searchable_attrs = { "text" },
	render = function(attrs)
		return string.format(
			'<a href="%s" class="btn btn--%s">%s</a>',
			attrs.url, attrs.style or "primary", attrs.text
		)
	end,
})

-- Inline: @mention pill
crap.richtext.register_node("mention", {
	label = "Mention",
	inline = true,
	attrs = {
		{ name = "name", type = "text", label = "Name", required = true },
		{ name = "user_id", type = "text", label = "User ID" },
	},
	searchable_attrs = { "name" },
	render = function(attrs)
		return string.format('<span class="mention">@%s</span>', attrs.name)
	end,
})

-- Global hook: log all content changes
---@param context crap.HookContext
---@return crap.HookContext
crap.hooks.register("after_change", function(context)
	local op = context.operation or "unknown"
	local collection = context.collection or "unknown"
	local id = context.data and context.data.id or "?"
	crap.log.info(string.format("[audit] %s/%s %s", collection, id, op))
	return context
end)

crap.log.info("Crap Studio ready")
