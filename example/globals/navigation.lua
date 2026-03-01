crap.globals.define("navigation", {
	labels = { singular = "Navigation" },
	fields = {
		{
			name = "main_nav",
			type = "array",
			admin = {
				label_field = "label",
				labels = {
					singular = { en = "Nav Item", de = "Navigationselement" },
					plural = { en = "Nav Items", de = "Navigationselemente" },
				},
			},
			fields = {
				{
					name = "label",
					type = "text",
					required = true,
					localized = true,
				},
				{
					name = "url",
					type = "text",
					required = true,
					admin = { placeholder = "/about" },
				},
				{
					name = "open_in_new_tab",
					type = "checkbox",
					default_value = false,
				},
				{
					name = "children",
					type = "array",
					admin = {
						label_field = "label",
						labels = { singular = "Sub Item", plural = "Sub Items" },
					},
					fields = {
						{
							name = "label",
							type = "text",
							required = true,
							localized = true,
						},
						{
							name = "url",
							type = "text",
							required = true,
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
