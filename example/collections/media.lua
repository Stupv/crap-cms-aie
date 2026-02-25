crap.collections.define("media", {
    labels = {
        singular = "Media",
        plural = "Media",
    },
    timestamps = true,
    upload = {
        mime_types = { "image/*", "application/pdf", "video/*" },
        image_sizes = {
            { name = "thumbnail", width = 300, height = 300, fit = "cover" },
            { name = "card", width = 640, height = 480, fit = "cover" },
            { name = "hero", width = 1920, height = 1080, fit = "cover" },
        },
        admin_thumbnail = "thumbnail",
    },
    admin = {
        use_as_title = "alt",
        default_sort = "-created_at",
    },
    fields = {
        {
            name = "alt",
            type = "text",
            required = true,
            admin = {
                description = "Describe the image for accessibility and SEO",
            },
        },
        {
            name = "caption",
            type = "text",
            admin = {
                description = "Optional caption displayed below the image",
            },
        },
    },
    access = {
        read = "access.anyone",
        create = "access.authenticated",
        update = "access.authenticated",
        delete = "access.admin_only",
    },
})
