crap.globals.define("site_settings", {
	labels = { singular = "Site Settings" },
	fields = {
		{
			name = "settings_tabs",
			type = "tabs",
			tabs = {
				{
					label = "General",
					fields = {
						{
							name = "site_name",
							type = "text",
							required = true,
							default_value = "Meridian Studio",
						},
						{
							name = "tagline",
							type = "text",
							admin = { placeholder = "Design. Build. Launch." },
						},
						{
							name = "contact_email",
							type = "email",
						},
						{
							name = "phone",
							type = "text",
						},
						{
							name = "address",
							type = "textarea",
							admin = { rows = 3 },
						},
					},
				},
				{
					label = "Branding",
					fields = {
						{
							name = "logo",
							type = "upload",
							relationship = { collection = "media" },
						},
						{
							name = "favicon",
							type = "upload",
							relationship = { collection = "media" },
						},
						{
							name = "primary_color",
							type = "text",
							default_value = "#2563eb",
							admin = { placeholder = "#hex" },
						},
						{
							name = "secondary_color",
							type = "text",
							default_value = "#7c3aed",
							admin = { placeholder = "#hex" },
						},
					},
				},
				{
					label = "Social",
					fields = {
						{
							name = "social",
							type = "group",
							fields = {
								{
									name = "github",
									type = "text",
									admin = { placeholder = "https://github.com/meridian" },
								},
								{
									name = "twitter",
									type = "text",
									admin = { placeholder = "https://twitter.com/meridian" },
								},
								{
									name = "linkedin",
									type = "text",
									admin = { placeholder = "https://linkedin.com/company/meridian" },
								},
								{
									name = "instagram",
									type = "text",
									admin = { placeholder = "https://instagram.com/meridian" },
								},
								{
									name = "youtube",
									type = "text",
									admin = { placeholder = "https://youtube.com/@meridian" },
								},
							},
						},
					},
				},
			},
		},
	},
	access = {
		read = "access.anyone",
		update = "access.admin_only",
	},
})
