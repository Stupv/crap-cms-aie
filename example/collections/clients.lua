crap.collections.define("clients", {
	labels = { singular = "Client", plural = "Clients" },
	timestamps = true,
	admin = {
		use_as_title = "company_name",
		default_sort = "company_name",
		list_searchable_fields = { "company_name", "contact_name", "contact_email" },
	},
	fields = {
		{
			name = "company_name",
			type = "text",
			required = true,
			admin = { placeholder = "Acme Corp" },
		},
		{
			name = "logo",
			type = "upload",
			relationship = { collection = "media" },
		},
		{
			name = "website",
			type = "text",
			admin = { placeholder = "https://..." },
		},
		{
			name = "since",
			type = "date",
			picker_appearance = "monthOnly",
			admin = { description = "Client since (month/year)" },
		},
		-- Contact info row
		{
			name = "contact_row",
			type = "row",
			fields = {
				{
					name = "contact_name",
					type = "text",
					admin = { width = "half", placeholder = "Primary contact" },
				},
				{
					name = "contact_email",
					type = "email",
					admin = { width = "half", placeholder = "contact@client.com" },
				},
			},
		},
		{
			name = "contact_phone",
			type = "text",
			admin = { placeholder = "+1 555 123 4567" },
		},
		{
			name = "industry",
			type = "select",
			options = {
				{ label = "Technology", value = "technology" },
				{ label = "Finance", value = "finance" },
				{ label = "Healthcare", value = "healthcare" },
				{ label = "Education", value = "education" },
				{ label = "Retail", value = "retail" },
				{ label = "Media", value = "media" },
				{ label = "Non-profit", value = "nonprofit" },
				{ label = "Government", value = "government" },
			},
		},
		{
			name = "notes",
			type = "textarea",
			admin = { rows = 4, description = "Internal notes about this client" },
		},
		-- Reverse relationship: projects for this client
		{
			name = "client_projects",
			type = "join",
			collection = "projects",
			on = "client",
		},
	},
	access = {
		read = "access.authenticated",
		create = "access.admin_or_director",
		update = "access.admin_or_director",
		delete = "access.admin_only",
	},
})
