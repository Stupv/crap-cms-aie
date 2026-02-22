--- @class Attribute
--- @field id number
--- @field item_id number
--- @field locale string
--- @field name string
--- @field value_int number
--- @field value_type string
--- @field value_float number
--- @field value_text string
--- @field value_bool boolean
--- @field created_at string
--- @field updated_at string

--- @class Item
--- @field id number
--- @field collection_id number
--- @field created_at string
--- @field updated_at string
--- @field attributes table<number, Attribute>

--- @class ItemWithAttributes
--- @field items table<number, Item>

--- @cast data Data
