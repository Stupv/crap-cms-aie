crap.collections.define("pages", {
	labels = { singular = "Page", plural = "Pages" },
	timestamps = true,
	versions = true,
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
			localized = true,
			admin = { placeholder = "Page title" },
		},
		{
			name = "slug",
			type = "text",
			required = true,
			unique = true,
			localized = true,
			hooks = {
				before_validate = { "hooks.auto_slug" },
			},
		},
		-- Page settings (tabs layout)
		{
			name = "page_settings",
			type = "tabs",
			tabs = {
				{
					label = "Content",
					fields = {
						{
							name = "content",
							type = "blocks",
							localized = true,
							blocks = {
								{
									type = "hero",
									label = "Hero Section",
									group = "Layout",
									fields = {
										{
											name = "heading",
											type = "text",
											required = true,
										},
										{
											name = "subheading",
											type = "text",
										},
										{
											name = "background",
											type = "upload",
											relationship = { collection = "media" },
										},
										{
											name = "cta_text",
											type = "text",
											admin = { placeholder = "Get in touch" },
										},
										{
											name = "cta_url",
											type = "text",
											admin = { placeholder = "/contact" },
										},
									},
								},
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
												},
											},
										},
									},
								},
								{
									type = "two_column",
									label = "Two Columns",
									group = "Layout",
									fields = {
										{
											name = "left",
											type = "richtext",
										},
										{
											name = "right",
											type = "richtext",
										},
									},
								},
								{
									type = "image_text",
									label = "Image + Text",
									group = "Layout",
									fields = {
										{
											name = "image",
											type = "upload",
											relationship = { collection = "media" },
										},
										{
											name = "body",
											type = "richtext",
										},
										{
											name = "image_position",
											type = "select",
											default_value = "left",
											options = {
												{ label = "Left", value = "left" },
												{ label = "Right", value = "right" },
											},
										},
									},
								},
								{
									type = "cta_banner",
									label = "CTA Banner",
									group = "Content",
									fields = {
										{
											name = "heading",
											type = "text",
											required = true,
										},
										{
											name = "description",
											type = "textarea",
										},
										{
											name = "button_text",
											type = "text",
										},
										{
											name = "button_url",
											type = "text",
										},
									},
								},
								{
									type = "team_grid",
									label = "Team Grid",
									group = "Content",
									fields = {
										{
											name = "heading",
											type = "text",
										},
										{
											name = "members",
											type = "relationship",
											relationship = { collection = "users", has_many = true },
										},
									},
								},
								{
									type = "services_list",
									label = "Services List",
									group = "Content",
									fields = {
										{
											name = "heading",
											type = "text",
										},
										{
											name = "services",
											type = "relationship",
											relationship = { collection = "services", has_many = true },
										},
									},
								},
							},
						},
					},
				},
				{
					label = "Settings",
					fields = {
						{
							name = "parent",
							type = "relationship",
							relationship = { collection = "pages" },
							admin = { description = "Parent page for nested navigation" },
						},
						{
							name = "template",
							type = "select",
							default_value = "default",
							options = {
								{ label = "Default", value = "default" },
								{ label = "Full Width", value = "full_width" },
								{ label = "Landing", value = "landing" },
								{ label = "Sidebar", value = "sidebar" },
							},
						},
						{
							name = "show_in_nav",
							type = "checkbox",
							default_value = true,
						},
						{
							name = "nav_order",
							type = "number",
							default_value = 0,
							admin = { step = "1" },
						},
					},
				},
			},
		},
	},
	hooks = {
		before_change = { "hooks.trim_title" },
	},
	access = {
		read = "access.anyone",
		create = "access.editor_or_above",
		update = "access.editor_or_above",
		delete = "access.admin_only",
	},
})
