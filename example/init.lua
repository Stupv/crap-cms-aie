crap.log.info("Crap CMS initializing...")

-- Global hook: audit log for all before_change events
crap.hooks.register("before_change", function(ctx)
    crap.log.info("[audit] " .. ctx.operation .. " on " .. ctx.collection)
    return ctx
end)

crap.log.info("init.lua loaded successfully")
