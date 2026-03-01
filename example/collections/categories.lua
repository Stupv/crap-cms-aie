crap.collections.define("categories", {
	labels = { singular = "Category", plural = "Categories" },
	timestamps = true,
	admin = {
		use_as_title = "title",
		default_sort = "title",
		list_searchable_fields = { "title", "slug" },
	},
	fields = {
		{
			name = "title",
			type = "text",
			required = true,
			hooks = {
				before_validate = { "hooks.trim_title" },
			},
			admin = { placeholder = "Category name" },
		},
		{
			name = "slug",
			type = "text",
			required = true,
			unique = true,
			hooks = {
				before_validate = { "hooks.auto_slug" },
			},
		},
		{
			name = "description",
			type = "textarea",
			admin = { rows = 2 },
		},
		{
			name = "parent",
			type = "relationship",
			relationship = { collection = "categories" },
			admin = { description = "Parent category for nesting" },
		},
		{
			name = "color",
			type = "text",
			admin = { placeholder = "#3b82f6" },
		},
	},
	access = {
		read = "access.anyone",
		create = "access.editor_or_above",
		update = "access.editor_or_above",
		delete = "access.admin_only",
	},
})
