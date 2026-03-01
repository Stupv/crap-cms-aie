crap.collections.define("services", {
	labels = {
		singular = { en = "Service", de = "Dienstleistung" },
		plural = { en = "Services", de = "Dienstleistungen" },
	},
	timestamps = true,
	admin = {
		use_as_title = "title",
		default_sort = "sort_order",
		list_searchable_fields = { "title" },
	},
	fields = {
		{
			name = "title",
			type = "text",
			required = true,
			localized = true,
			admin = {
				label = { en = "Title", de = "Titel" },
				placeholder = "Service name",
			},
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
			type = "textarea",
			localized = true,
			admin = {
				label = { en = "Description", de = "Beschreibung" },
				rows = 4,
			},
		},
		{
			name = "icon",
			type = "code",
			admin = {
				language = "html",
				description = "SVG icon markup",
			},
		},
		{
			name = "active",
			type = "checkbox",
			default_value = true,
			admin = { position = "sidebar" },
		},
		{
			name = "sort_order",
			type = "number",
			default_value = 0,
			admin = { position = "sidebar", step = "1" },
		},
		-- Pricing
		{
			name = "pricing_type",
			type = "radio",
			required = true,
			default_value = "fixed",
			options = {
				{ label = "Fixed Price", value = "fixed" },
				{ label = "Hourly Rate", value = "hourly" },
				{ label = "Custom Quote", value = "custom" },
			},
		},
		{
			name = "price_range",
			type = "group",
			admin = {
				label = "Price Range",
				condition = "hooks.conditions.show_price_range",
			},
			fields = {
				{
					name = "min_price",
					type = "number",
					min = 0,
					admin = { placeholder = "From", width = "half" },
				},
				{
					name = "max_price",
					type = "number",
					min = 0,
					admin = { placeholder = "To", width = "half" },
				},
				{
					name = "currency",
					type = "select",
					default_value = "USD",
					options = {
						{ label = "USD", value = "USD" },
						{ label = "EUR", value = "EUR" },
						{ label = "GBP", value = "GBP" },
					},
				},
			},
		},
		-- Features array
		{
			name = "features",
			type = "array",
			admin = {
				label_field = "title",
				labels = { singular = "Feature", plural = "Features" },
			},
			fields = {
				{
					name = "title",
					type = "text",
					required = true,
				},
				{
					name = "included",
					type = "checkbox",
					default_value = true,
				},
			},
		},
		{
			name = "hero_image",
			type = "upload",
			relationship = { collection = "media" },
		},
	},
	access = {
		read = "access.anyone",
		create = "access.admin_or_director",
		update = "access.admin_or_director",
		delete = "access.admin_only",
	},
})
