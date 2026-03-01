crap.collections.define("posts", {
	labels = { singular = "Post", plural = "Posts" },
	timestamps = true,
	versions = true,
	live = true,
	admin = {
		use_as_title = "title",
		default_sort = "-published_at",
		list_searchable_fields = { "title", "slug", "excerpt" },
	},
	fields = {
		{
			name = "title",
			type = "text",
			required = true,
			hooks = {
				before_validate = { "hooks.trim_title" },
			},
			admin = { placeholder = "Post title" },
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
			name = "excerpt",
			type = "textarea",
			admin = { rows = 3, placeholder = "Brief summary for listings and SEO" },
		},
		-- Sidebar fields
		{
			name = "post_type",
			type = "select",
			required = true,
			default_value = "article",
			options = {
				{ label = "Article", value = "article" },
				{ label = "Case Study", value = "case_study" },
				{ label = "Link", value = "link" },
				{ label = "Video", value = "video" },
			},
			admin = { position = "sidebar" },
		},
		{
			name = "published_at",
			type = "date",
			picker_appearance = "dayAndTime",
			admin = { position = "sidebar" },
		},
		{
			name = "external_url",
			type = "text",
			admin = {
				placeholder = "https://...",
				condition = {
					field = "post_type",
					condition = "one_of",
					value = { "link", "video" },
				},
			},
		},
		-- Relationships
		{
			name = "author",
			type = "relationship",
			required = true,
			relationship = { collection = "users" },
		},
		{
			name = "hero_image",
			type = "upload",
			relationship = { collection = "media" },
		},
		{
			name = "categories",
			type = "relationship",
			relationship = { collection = "categories", has_many = true },
		},
		{
			name = "tags",
			type = "relationship",
			relationship = { collection = "tags", has_many = true },
		},
		-- Content
		{
			name = "content",
			type = "richtext",
			admin = {
				features = {
					"bold",
					"italic",
					"link",
					"heading",
					"blockquote",
					"bulletList",
					"orderedList",
					"code",
					"codeBlock",
					"horizontalRule",
				},
			},
		},
		-- Reading time (virtual, computed by after_read hook)
		{
			name = "reading_time",
			type = "text",
			admin = { readonly = true, position = "sidebar" },
			hooks = {
				after_read = { "hooks.reading_time" },
			},
		},
		-- Polymorphic relationship: related posts OR projects
		{
			name = "related_content",
			type = "relationship",
			relationship = {
				collection = { "posts", "projects" },
				has_many = true,
				max_depth = 1,
			},
			admin = { description = "Related posts or projects" },
		},
		-- Publishing collapsible
		{
			name = "publishing",
			type = "collapsible",
			admin = { label = "Publishing", collapsed = true },
			fields = {
				{
					name = "featured",
					type = "checkbox",
					default_value = false,
				},
				{
					name = "pinned",
					type = "checkbox",
					default_value = false,
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
		delete = "access.editor_or_above",
	},
})
