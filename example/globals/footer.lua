crap.globals.define("footer", {
	labels = { singular = "Footer" },
	fields = {
		{
			name = "copyright_text",
			type = "text",
			localized = true,
			default_value = "Meridian Studio. All rights reserved.",
		},
		{
			name = "show_social_links",
			type = "checkbox",
			default_value = true,
		},
		{
			name = "partner_logos",
			type = "array",
			admin = {
				label_field = "name",
				labels = { singular = "Partner", plural = "Partners" },
			},
			fields = {
				{
					name = "name",
					type = "text",
					required = true,
				},
				{
					name = "logo",
					type = "upload",
					relationship = { collection = "media" },
				},
				{
					name = "url",
					type = "text",
					admin = { placeholder = "https://..." },
				},
			},
		},
	},
	access = {
		read = "access.anyone",
		update = "access.admin_only",
	},
})
