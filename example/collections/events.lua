crap.collections.define("events", {
	labels = { singular = "Event", plural = "Events" },
	timestamps = true,
	admin = {
		use_as_title = "title",
		default_sort = "-start_date",
		list_searchable_fields = { "title" },
	},
	fields = {
		{
			name = "title",
			type = "text",
			required = true,
			admin = { placeholder = "Event title" },
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
			name = "description",
			type = "richtext",
			admin = {
				features = { "bold", "italic", "link", "bulletList" },
			},
		},
		{
			name = "hero_image",
			type = "upload",
			relationship = { collection = "media" },
		},
		-- Date row
		{
			name = "date_row",
			type = "row",
			fields = {
				{
					name = "start_date",
					type = "date",
					required = true,
					picker_appearance = "dayAndTime",
					admin = { width = "half" },
				},
				{
					name = "end_date",
					type = "date",
					picker_appearance = "dayAndTime",
					admin = { width = "half" },
				},
			},
		},
		-- Online toggle + conditional URL
		{
			name = "online",
			type = "checkbox",
			default_value = false,
		},
		{
			name = "event_url",
			type = "text",
			admin = {
				placeholder = "https://zoom.us/...",
				condition = {
					field = "online",
					condition = "equals",
					value = true,
				},
			},
		},
		-- Location group
		{
			name = "location",
			type = "group",
			admin = { label = "Venue" },
			fields = {
				{
					name = "venue_name",
					type = "text",
					admin = { placeholder = "Venue name" },
				},
				{
					name = "address",
					type = "text",
					admin = { placeholder = "123 Main St" },
				},
				{
					name = "city",
					type = "text",
					admin = { width = "half" },
				},
				{
					name = "country",
					type = "text",
					admin = { width = "half" },
				},
			},
		},
		-- Speakers (drawer picker)
		{
			name = "speakers",
			type = "relationship",
			relationship = { collection = "users", has_many = true },
			admin = {
				picker = "drawer",
				description = "Event speakers / presenters",
			},
		},
		{
			name = "categories",
			type = "relationship",
			relationship = { collection = "categories", has_many = true },
		},
		-- Registration (collapsible)
		{
			name = "registration",
			type = "collapsible",
			admin = { label = "Registration", collapsed = true },
			fields = {
				{
					name = "registration_url",
					type = "text",
					admin = { placeholder = "https://..." },
				},
				{
					name = "max_attendees",
					type = "number",
					min = 0,
					admin = { step = "1" },
				},
				{
					name = "registration_deadline",
					type = "date",
					picker_appearance = "dayAndTime",
				},
			},
		},
	},
	access = {
		read = "access.anyone",
		create = "access.editor_or_above",
		update = "access.editor_or_above",
		delete = "access.admin_or_director",
	},
})
