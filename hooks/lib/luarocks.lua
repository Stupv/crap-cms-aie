local lib = {}

-- Function to install a Lua library using LuaRocks
function lib.install_luarock(rock_name)
  local command = "luarocks --tree=" .. rocks_dir .. " install " .. rock_name

  local handle = io.popen(command)

  if not handle then
    error("Failed to run command: " .. command)
  end

  local _ = handle:read("*a")
  handle:close()
end

return lib
