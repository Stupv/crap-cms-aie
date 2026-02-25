crap.jobs.define("cleanup_expired", {
    handler = "jobs.cleanup_expired.run",
    schedule = "0 3 * * *",
    queue = "maintenance",
    retries = 3,
    timeout = 300,
    labels = { singular = "Cleanup Expired" },
})

local M = {}

--- Delete posts with status "archived" that were updated more than 30 days ago.
---@param ctx crap.JobHandlerContext
---@return table?
function M.run(ctx)
    local cutoff = crap.util.date_format(
        crap.util.date_add(crap.util.date_timestamp(), -30 * 86400),
        "%Y-%m-%dT%H:%M:%SZ"
    )

    local result = crap.collections.find("posts", {
        filters = {
            status = "archived",
            updated_at = { less_than = cutoff },
        },
    })

    local deleted = 0
    for _, doc in ipairs(result.documents) do
        crap.collections.delete("posts", doc.id)
        deleted = deleted + 1
    end

    crap.log.info("Cleanup expired: deleted " .. deleted .. " archived posts")
    return { deleted = deleted }
end

return M
