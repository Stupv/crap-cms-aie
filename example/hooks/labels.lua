-- hooks/labels.lua — Row label functions for admin UI
-- These are pure formatting functions (no DB access).
-- They receive the row data as a Lua table and return a display string.

local M = {}

--- Computed row label for post content blocks.
--- Priority: row_label function > per-block label_field > block type + index.
---@param row table Row data including `_block_type` and block field values.
---@return string?
function M.content_block_row(row)
    local bt = row._block_type or ""
    if bt == "richtext" then
        -- Rich text blocks don't have a good title field, so use a fixed label
        return "Rich Text"
    elseif bt == "image" then
        local caption = row.caption or ""
        if caption ~= "" then
            return "Image: " .. caption
        end
        return "Image"
    elseif bt == "code" then
        local lang = row.language or ""
        if lang ~= "" then
            return "Code (" .. lang .. ")"
        end
        return "Code"
    elseif bt == "quote" then
        local attr = row.attribution or ""
        if attr ~= "" then
            return "Quote — " .. attr
        end
        return "Quote"
    end
    return nil -- fall back to label_field or default
end

return M
