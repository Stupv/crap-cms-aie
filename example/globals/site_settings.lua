crap.globals.define("site_settings", {
    labels = {
        singular = "Site Settings",
    },
    fields = {
        {
            name = "site_name",
            type = "text",
            required = true,
            default_value = "My Site",
        },
        {
            name = "tagline",
            type = "text",
        },
    },
})
