crap.collections.define("users", {
	labels = { singular = "User", plural = "Users" },
	timestamps = true,
	auth = {
		verify_email = false,
		strategies = {
			{
				name = "api-key",
				authenticate = "access.api_key_strategy",
			},
		},
	},
	admin = {
		use_as_title = "name",
		default_sort = "-created_at",
		list_searchable_fields = { "name", "email" },
	},
	fields = {
		{
			name = "name",
			type = "text",
			required = true,
			admin = { placeholder = "Full name" },
		},
		{
			name = "role",
			type = "select",
			required = true,
			default_value = "author",
			options = {
				{ label = "Admin", value = "admin" },
				{ label = "Director", value = "director" },
				{ label = "Editor", value = "editor" },
				{ label = "Author", value = "author" },
			},
			admin = { position = "sidebar" },
		},
		{
			name = "skills",
			type = "select",
			has_many = true,
			options = {
				{ label = "Design", value = "design" },
				{ label = "Development", value = "development" },
				{ label = "Strategy", value = "strategy" },
				{ label = "Motion", value = "motion" },
				{ label = "Photography", value = "photography" },
				{ label = "Copywriting", value = "copywriting" },
				{ label = "3D", value = "3d" },
			},
			admin = { description = "Areas of expertise" },
		},
		{
			name = "avatar",
			type = "upload",
			relationship = { collection = "media" },
		},
		{
			name = "bio",
			type = "textarea",
			admin = { rows = 4 },
		},
		{
			name = "authored_posts",
			type = "join",
			collection = "posts",
			on = "author",
		},
	},
	hooks = {
		before_delete = { "hooks.prevent_last_admin" },
	},
	access = {
		read = "access.anyone",
		create = "access.admin_only",
		update = "access.self_or_admin",
		delete = "access.admin_only",
	},
})
