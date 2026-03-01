crap.collections.define("testimonials", {
	labels = { singular = "Testimonial", plural = "Testimonials" },
	timestamps = true,
	admin = {
		use_as_title = "author_name",
		default_sort = "-created_at",
		list_searchable_fields = { "author_name", "company" },
	},
	fields = {
		{
			name = "author_name",
			type = "text",
			required = true,
			admin = { placeholder = "Client name" },
		},
		{
			name = "author_title",
			type = "text",
			admin = { placeholder = "CEO, Acme Corp" },
		},
		{
			name = "company",
			type = "text",
		},
		{
			name = "author_photo",
			type = "upload",
			relationship = { collection = "media" },
		},
		{
			name = "quote",
			type = "textarea",
			required = true,
			admin = { rows = 4, placeholder = "What the client said..." },
		},
		{
			name = "rating",
			type = "number",
			required = true,
			min = 1,
			max = 5,
			default_value = 5,
			admin = {
				step = "1",
				description = "Rating from 1 to 5",
			},
		},
		{
			name = "project",
			type = "relationship",
			relationship = { collection = "projects" },
			admin = { description = "Related project" },
		},
		{
			name = "featured",
			type = "checkbox",
			default_value = false,
			admin = { position = "sidebar" },
		},
	},
	access = {
		read = "access.anyone",
		create = "access.editor_or_above",
		update = "access.editor_or_above",
		delete = "access.admin_only",
	},
})
