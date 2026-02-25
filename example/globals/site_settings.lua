crap.globals.define("site_settings", {
    labels = {
        singular = "Site Settings",
    },
    fields = {
        {
            name = "site_name",
            type = "text",
            required = true,
            default_value = "Six Seven",
            admin = {
                description = "Displayed in the header and browser tab",
            },
        },
        {
            name = "tagline",
            type = "text",
            default_value = "Where six meets seven",
            admin = {
                description = "Short tagline shown below the site name",
            },
        },
        {
            name = "description",
            type = "textarea",
            admin = {
                description = "Default site description for SEO",
            },
        },
        {
            name = "logo",
            type = "upload",
            relationship = { collection = "media" },
            admin = {
                width = "half",
            },
        },
        {
            name = "favicon",
            type = "upload",
            relationship = { collection = "media" },
            admin = {
                width = "half",
            },
        },
        -- Social links group
        {
            name = "social",
            type = "group",
            admin = {
                label = "Social Links",
                collapsed = true,
            },
            fields = {
                {
                    name = "github",
                    type = "text",
                    admin = { placeholder = "https://github.com/...", width = "half" },
                },
                {
                    name = "twitter",
                    type = "text",
                    admin = { placeholder = "https://x.com/...", width = "half" },
                },
                {
                    name = "mastodon",
                    type = "text",
                    admin = { placeholder = "https://mastodon.social/@...", width = "half" },
                },
                {
                    name = "bluesky",
                    type = "text",
                    admin = { placeholder = "https://bsky.app/profile/...", width = "half" },
                },
            },
        },
        -- Default SEO group
        {
            name = "seo",
            type = "group",
            admin = {
                label = "Default SEO",
                description = "Fallback values when pages don't have their own SEO settings",
                collapsed = true,
            },
            fields = {
                {
                    name = "default_title_suffix",
                    type = "text",
                    default_value = " | Six Seven",
                    admin = {
                        label = "Title Suffix",
                        description = "Appended to every page title (e.g., ' | Six Seven')",
                    },
                },
                {
                    name = "default_og_image",
                    type = "upload",
                    relationship = { collection = "media" },
                    admin = {
                        label = "Default Social Image",
                        description = "Used when a page has no custom social image",
                    },
                },
            },
        },
    },
    access = {
        read = "access.anyone",
        update = "access.admin_only",
    },
})
