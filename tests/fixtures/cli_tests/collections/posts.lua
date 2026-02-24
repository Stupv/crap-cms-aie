crap.collections.define("posts", {
    labels = {
        singular = "Post",
        plural = "Posts",
    },
    timestamps = true,
    admin = {
        use_as_title = "title",
    },
    fields = {
        {
            name = "title",
            type = "text",
            required = true,
        },
        {
            name = "status",
            type = "select",
            default_value = "draft",
            options = {
                { label = "Draft", value = "draft" },
                { label = "Published", value = "published" },
            },
        },
        {
            name = "content",
            type = "richtext",
        },
    },
})
