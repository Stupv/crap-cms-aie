require "type_hints"

local lib_dump = require "lib.dump"

print("Lua version:", _VERSION)

--- After get items hook - runs directly after retrieving items
--- @param data ItemWithAttributes
--- @return ItemWithAttributes
function after_get_items_hook(data)
  if not type(data) == "table" then
    print(api.say_hello('Daniel'))
    print(data)

    return data
  end

  -- Modify data
  for i, _ in pairs(data) do
    if data[i].attributes then
      for j, _ in pairs(data[i].attributes) do
        -- Change language
        if data[i].attributes[j].locale == "en" then
          data[i].attributes[j].locale = "de"
        end

        -- Add fallback text
        if data[i].attributes[j].value_type == "Text" and data[i].attributes[j].value_text == "" then
          data[i].attributes[j].value_text = "Fallback Text"
        end
      end
    end
  end

  print(api.say_hello('Daniel'))
  print(lib_dump.dump(data))

  return data
end
