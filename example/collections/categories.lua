crap.collections.define("categories", {
    labels = {
        singular = "Category",
        plural = "Categories",
    },
    timestamps = true,
    admin = {
        use_as_title = "title",
        default_sort = "title",
    },
    fields = {
        {
            name = "title",
            type = "text",
            required = true,
            unique = true,
        },
        {
            name = "slug",
            type = "text",
            required = true,
            unique = true,
            admin = {
                description = "URL-safe identifier (auto-generated from title)",
            },
            hooks = {
                before_validate = { "hooks.auto_slug" },
            },
        },
        {
            name = "description",
            type = "textarea",
            admin = {
                description = "Describe what this category covers",
            },
        },
    },
    hooks = {
        before_change = { "hooks.trim_title" },
    },
    access = {
        read = "access.anyone",
        create = "access.editor_or_admin",
        update = "access.editor_or_admin",
        delete = "access.admin_only",
    },
})
