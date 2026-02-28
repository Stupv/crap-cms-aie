crap.globals.define("site_settings", {
    labels = {
        singular = "Site Settings",
    },
    fields = {
        -- Tabs: organizes settings into General and Branding sections
        {
            name = "settings_tabs",
            type = "tabs",
            tabs = {
                {
                    label = "General",
                    description = "Core site identity and social links",
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
                        -- Social links group (nested inside General tab)
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
                    },
                },
                {
                    label = "Branding",
                    description = "Logos, images, and SEO defaults",
                    fields = {
                        {
                            name = "logo",
                            type = "upload",
                            relationship = { collection = "media" },
                            admin = {
                                width = "half",
                                description = "Primary site logo",
                            },
                        },
                        {
                            name = "favicon",
                            type = "upload",
                            relationship = { collection = "media" },
                            admin = {
                                width = "half",
                                description = "Browser tab icon (recommended: 32x32 PNG)",
                            },
                        },
                        -- Default SEO collapsible (nested inside Branding tab)
                        {
                            name = "seo_defaults",
                            type = "collapsible",
                            admin = {
                                label = "Default SEO",
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
                },
            },
        },
    },
    access = {
        read = "access.anyone",
        update = "access.admin_only",
    },
})
