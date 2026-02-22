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

--- @class Data
--- @field items table<number, Item>

--- @cast data Data

local lib_dump = require "lib.dump"

for i, v in pairs(data.items) do
  if data.items[i].attributes then
    for j, k in pairs(data.items[i].attributes) do
      -- Change language
      if data.items[i].attributes[j].locale == "en" then
        data.items[i].attributes[j].locale = "de"
      end

      -- Add fallback text
      if data.items[i].attributes[j].value_type == "Text" and data.items[i].attributes[j].value_text == "" then
        data.items[i].attributes[j].value_text = "Fallback Text"
      end
    end
  end
end



print("Lua version:", _VERSION)
print("From hook")
print(lib_dump.dump(data))
print("End from hook")

print(api.say_hello('Daniel'))
