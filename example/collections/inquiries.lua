crap.collections.define("inquiries", {
	labels = { singular = "Inquiry", plural = "Inquiries" },
	timestamps = true,
	admin = {
		use_as_title = "name",
		default_sort = "-created_at",
		list_searchable_fields = { "name", "email", "company" },
	},
	fields = {
		{
			name = "name",
			type = "text",
			required = true,
			admin = { placeholder = "Full name" },
		},
		{
			name = "email",
			type = "email",
			required = true,
		},
		{
			name = "company",
			type = "text",
		},
		{
			name = "phone",
			type = "text",
		},
		{
			name = "service",
			type = "relationship",
			relationship = { collection = "services" },
			admin = { description = "Service they're interested in" },
		},
		{
			name = "budget_range",
			type = "select",
			options = {
				{ label = "Under $5,000", value = "under_5k" },
				{ label = "$5,000 - $15,000", value = "5k_15k" },
				{ label = "$15,000 - $50,000", value = "15k_50k" },
				{ label = "$50,000 - $100,000", value = "50k_100k" },
				{ label = "Over $100,000", value = "over_100k" },
			},
		},
		{
			name = "message",
			type = "textarea",
			required = true,
			admin = { rows = 6 },
		},
		{
			name = "status",
			type = "select",
			required = true,
			default_value = "new",
			options = {
				{ label = "New", value = "new" },
				{ label = "Contacted", value = "contacted" },
				{ label = "Qualified", value = "qualified" },
				{ label = "Proposal Sent", value = "proposal" },
				{ label = "Won", value = "won" },
				{ label = "Lost", value = "lost" },
				{ label = "Archived", value = "archived" },
			},
			admin = { position = "sidebar" },
		},
		{
			name = "assigned_to",
			type = "relationship",
			relationship = { collection = "users" },
			admin = { position = "sidebar" },
		},
		-- Internal notes (field-level access: admin/director only)
		{
			name = "internal_notes",
			type = "textarea",
			admin = { rows = 4, description = "Internal notes (admin/director only)" },
			access = {
				read = "access.field_admin_or_director",
				create = "access.field_admin_or_director",
				update = "access.field_admin_or_director",
			},
		},
		-- JSON metadata for tracking
		{
			name = "metadata",
			type = "json",
			admin = {
				description = "Tracking data (UTM params, referrer, etc.)",
				language = "json",
			},
		},
	},
	hooks = {
		after_change = { "hooks.notify_inquiry" },
	},
	access = {
		read = "access.authenticated",
		create = "access.anyone",
		update = "access.editor_or_above",
		delete = "access.admin_only",
	},
})
