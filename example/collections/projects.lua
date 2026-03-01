crap.collections.define("projects", {
	labels = { singular = "Project", plural = "Projects" },
	timestamps = true,
	versions = true,
	live = true,
	admin = {
		use_as_title = "title",
		default_sort = "-created_at",
		list_searchable_fields = { "title", "slug", "excerpt" },
	},
	fields = {
		{
			name = "title",
			type = "text",
			required = true,
			localized = true,
			hooks = {
				before_validate = { "hooks.trim_title" },
			},
			admin = { placeholder = "Project title" },
		},
		{
			name = "slug",
			type = "text",
			required = true,
			unique = true,
			hooks = {
				before_validate = { "hooks.auto_slug" },
			},
			admin = { placeholder = "auto-generated-from-title" },
		},
		{
			name = "excerpt",
			type = "textarea",
			admin = { rows = 3, placeholder = "Brief project description" },
		},
		-- Sidebar fields
		{
			name = "status",
			type = "select",
			required = true,
			default_value = "planning",
			options = {
				{ label = "Planning", value = "planning" },
				{ label = "In Progress", value = "in_progress" },
				{ label = "Review", value = "review" },
				{ label = "Completed", value = "completed" },
				{ label = "Archived", value = "archived" },
			},
			admin = { position = "sidebar" },
		},
		{
			name = "priority",
			type = "radio",
			default_value = "normal",
			options = {
				{ label = "Low", value = "low" },
				{ label = "Normal", value = "normal" },
				{ label = "High", value = "high" },
				{ label = "Urgent", value = "urgent" },
			},
			admin = { position = "sidebar" },
		},
		{
			name = "featured",
			type = "checkbox",
			default_value = false,
			admin = { position = "sidebar" },
		},
		-- Dates row
		{
			name = "dates_row",
			type = "row",
			fields = {
				{
					name = "start_date",
					type = "date",
					picker_appearance = "dayOnly",
					admin = { width = "half" },
				},
				{
					name = "end_date",
					type = "date",
					picker_appearance = "dayOnly",
					admin = {
						width = "half",
						condition = {
							field = "status",
							condition = "not_equals",
							value = "planning",
						},
					},
				},
			},
		},
		-- Relationships
		{
			name = "hero_image",
			type = "upload",
			relationship = { collection = "media" },
		},
		{
			name = "client",
			type = "relationship",
			relationship = { collection = "clients" },
		},
		{
			name = "team",
			type = "relationship",
			relationship = { collection = "users", has_many = true },
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
		-- Budget (field-level access)
		{
			name = "budget",
			type = "number",
			min = 0,
			hooks = {
				before_validate = { "hooks.validate_budget" },
			},
			access = {
				read = "access.field_admin_or_director",
				create = "access.field_admin_or_director",
				update = "access.field_admin_or_director",
			},
			admin = { description = "Project budget (visible to admin/director only)" },
		},
		-- Deliverables array
		{
			name = "deliverables",
			type = "array",
			admin = {
				label_field = "title",
				labels = { singular = "Deliverable", plural = "Deliverables" },
			},
			fields = {
				{
					name = "title",
					type = "text",
					required = true,
				},
				{
					name = "completed",
					type = "checkbox",
					default_value = false,
				},
			},
		},
		-- Content blocks
		{
			name = "content",
			type = "blocks",
			admin = {
				picker = "card",
				row_label = "hooks.labels",
				init_collapsed = true,
			},
			blocks = {
				{
					type = "richtext",
					label = "Rich Text",
					group = "Content",
					fields = {
						{
							name = "body",
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
								},
							},
						},
					},
				},
				{
					type = "image_gallery",
					label = "Image Gallery",
					label_field = "caption",
					group = "Media",
					fields = {
						{
							name = "caption",
							type = "text",
						},
						{
							name = "images",
							type = "upload",
							relationship = { collection = "media", has_many = true },
						},
						{
							name = "columns",
							type = "select",
							default_value = "3",
							options = {
								{ label = "2 Columns", value = "2" },
								{ label = "3 Columns", value = "3" },
								{ label = "4 Columns", value = "4" },
							},
						},
					},
				},
				{
					type = "video_embed",
					label = "Video Embed",
					group = "Media",
					fields = {
						{
							name = "url",
							type = "text",
							required = true,
							admin = { placeholder = "https://youtube.com/watch?v=..." },
						},
						{
							name = "caption",
							type = "text",
						},
					},
				},
				{
					type = "stats",
					label = "Stats Row",
					group = "Content",
					fields = {
						{
							name = "items",
							type = "array",
							min_rows = 1,
							max_rows = 4,
							fields = {
								{
									name = "value",
									type = "text",
									required = true,
									admin = { placeholder = "98%" },
								},
								{
									name = "label",
									type = "text",
									required = true,
									admin = { placeholder = "Client satisfaction" },
								},
							},
						},
					},
				},
				{
					type = "testimonial",
					label = "Testimonial",
					group = "Content",
					fields = {
						{
							name = "quote",
							type = "textarea",
							required = true,
						},
						{
							name = "author_name",
							type = "text",
							required = true,
						},
						{
							name = "author_title",
							type = "text",
						},
					},
				},
				{
					type = "code_block",
					label = "Code",
					group = "Technical",
					fields = {
						{
							name = "code",
							type = "code",
							admin = { language = "javascript" },
						},
						{
							name = "caption",
							type = "text",
						},
					},
				},
			},
		},
		-- Publishing options (collapsible)
		{
			name = "publishing_options",
			type = "collapsible",
			admin = { label = "Publishing Options", collapsed = true },
			fields = {
				{
					name = "published_at",
					type = "date",
					picker_appearance = "dayAndTime",
				},
				{
					name = "external_url",
					type = "text",
					admin = { placeholder = "https://..." },
				},
			},
		},
	},
	hooks = {
		before_change = { "hooks.set_published_at" },
	},
	access = {
		read = "access.anyone",
		create = "access.editor_or_above",
		update = "access.team_or_admin",
		delete = "access.admin_or_director",
	},
})
