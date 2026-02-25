crap.collections.define("posts", {
    labels = {
        singular = "Post",
        plural = "Posts",
    },
    timestamps = true,
    versions = true,
    live = true,
    admin = {
        use_as_title = "title",
        default_sort = "-created_at",
        list_searchable_fields = { "title", "excerpt" },
    },
    fields = {
        {
            name = "title",
            type = "text",
            required = true,
            admin = {
                placeholder = "Enter post title...",
            },
            hooks = {
                before_validate = { "hooks.trim_title" },
            },
        },
        {
            name = "slug",
            type = "text",
            required = true,
            unique = true,
            admin = {
                description = "URL-safe identifier (auto-generated from title)",
                width = "half",
            },
            hooks = {
                before_validate = { "hooks.auto_slug" },
            },
        },
        {
            name = "author",
            type = "relationship",
            required = true,
            relationship = {
                collection = "users",
            },
            admin = {
                width = "half",
            },
        },
        {
            name = "featured_image",
            type = "upload",
            relationship = {
                collection = "media",
            },
            admin = {
                description = "Main image shown in cards and at the top of the post",
            },
        },
        {
            name = "excerpt",
            type = "textarea",
            required = true,
            admin = {
                description = "Short summary for cards and SEO (max 160 characters)",
                placeholder = "A brief summary of this post...",
            },
        },
        {
            name = "content",
            type = "blocks",
            blocks = {
                {
                    type = "richtext",
                    label = "Rich Text",
                    fields = {
                        { name = "body", type = "richtext" },
                    },
                },
                {
                    type = "image",
                    label = "Image",
                    fields = {
                        {
                            name = "image",
                            type = "upload",
                            required = true,
                            relationship = { collection = "media" },
                        },
                        { name = "caption", type = "text" },
                    },
                },
                {
                    type = "code",
                    label = "Code",
                    fields = {
                        {
                            name = "language",
                            type = "select",
                            options = {
                                { label = "JavaScript", value = "javascript" },
                                { label = "TypeScript", value = "typescript" },
                                { label = "Rust", value = "rust" },
                                { label = "Python", value = "python" },
                                { label = "Lua", value = "lua" },
                                { label = "HTML", value = "html" },
                                { label = "CSS", value = "css" },
                                { label = "Shell", value = "shell" },
                            },
                        },
                        { name = "code", type = "textarea", required = true },
                    },
                },
                {
                    type = "quote",
                    label = "Quote",
                    fields = {
                        { name = "text", type = "textarea", required = true },
                        { name = "attribution", type = "text" },
                    },
                },
            },
        },
        {
            name = "category",
            type = "relationship",
            relationship = {
                collection = "categories",
            },
            admin = {
                width = "half",
                position = "sidebar",
            },
        },
        {
            name = "tags",
            type = "relationship",
            relationship = {
                collection = "tags",
                has_many = true,
            },
            admin = {
                position = "sidebar",
            },
        },
        {
            name = "published_at",
            type = "date",
            picker_appearance = "dayAndTime",
            admin = {
                description = "Schedule publication (defaults to now when published)",
                width = "half",
                position = "sidebar",
            },
        },
        -- SEO group
        {
            name = "seo",
            type = "group",
            admin = {
                label = "SEO",
                description = "Search engine optimization settings",
                collapsed = true,
                position = "sidebar",
            },
            fields = {
                {
                    name = "meta_title",
                    type = "text",
                    admin = {
                        label = "Meta Title",
                        description = "Override the default page title for search engines",
                        placeholder = "Custom SEO title...",
                    },
                },
                {
                    name = "meta_description",
                    type = "textarea",
                    admin = {
                        label = "Meta Description",
                        description = "Appears in search result snippets (max 160 chars)",
                        placeholder = "Describe this page for search engines...",
                    },
                },
                {
                    name = "og_image",
                    type = "upload",
                    relationship = { collection = "media" },
                    admin = {
                        label = "Social Image",
                        description = "Image shown when shared on social media (1200x630 recommended)",
                    },
                },
                {
                    name = "no_index",
                    type = "checkbox",
                    default_value = false,
                    admin = {
                        label = "No Index",
                        description = "Hide this page from search engines",
                    },
                },
            },
        },
    },
    hooks = {
        before_change = { "hooks.set_published_at" },
    },
    access = {
        read = "access.published_or_author",
        create = "access.authenticated",
        update = "access.author_or_editor",
        delete = "access.author_or_admin",
    },
})
