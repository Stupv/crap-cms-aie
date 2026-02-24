local M = {}

function M.up()
	-- TODO: implement migration
	-- crap.* API available (find, create, update, delete)
	crap.log.info("Migration foo barr")
end

function M.down()
	-- TODO: implement rollback (best-effort)
end

return M
