crap.collections.define("tags", {
	labels = {
		singular = "Tag",
		plural = "Tags",
	},
	timestamps = true,
	admin = {
		use_as_title = "name",
		default_sort = "name",
	},
	fields = {
		{
			name = "name",
			type = "text",
			required = true,
			unique = true,
		},
		{
			name = "color",
			type = "text",
			admin = {
				placeholder = "#ff0000",
				description = "Hex color code for tag badge",
			},
		},
	},
})
