crap.collections.define("pages", {
	labels = {
		singular = "Page",
		plural = "Pages",
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
		},
		{
			name = "slug",
			type = "text",
			required = true,
			unique = true,
		},
		{
			name = "body",
			type = "textarea",
		},
		{
			name = "published",
			type = "checkbox",
			default_value = false,
		},
	},
})
