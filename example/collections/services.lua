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
    crap.fields.text({
      name = "title",
      required = true,
      localized = true,
      admin = { label = { en = "Title", de = "Titel" }, placeholder = "Service name" },
    }),
    crap.fields.text({
      name = "slug",
      required = true,
      unique = true,
      hooks = { before_validate = { "hooks.auto_slug" } },
    }),
    crap.fields.textarea({
      name = "description",
      localized = true,
      admin = { label = { en = "Description", de = "Beschreibung" }, rows = 4 },
    }),
    crap.fields.code({
      name = "icon",
      admin = { language = "html", description = "SVG icon markup" },
    }),
    crap.fields.checkbox({ name = "active", default_value = true, admin = { position = "sidebar" } }),
    crap.fields.number({
      name = "sort_order",
      default_value = 0,
      admin = { position = "sidebar", step = "1" },
    }),
    -- Pricing
    crap.fields.radio({
      name = "pricing_type",
      required = true,
      default_value = "fixed",
      options = {
        { label = "Fixed Price", value = "fixed" },
        { label = "Hourly Rate", value = "hourly" },
        { label = "Custom Quote", value = "custom" },
      },
    }),
    crap.fields.group({
      name = "price_range",
      admin = { label = "Price Range", condition = "hooks.conditions.show_price_range" },
      fields = {
        crap.fields.number({ name = "min_price", min = 0, admin = { placeholder = "From", width = "half" } }),
        crap.fields.number({ name = "max_price", min = 0, admin = { placeholder = "To", width = "half" } }),
        crap.fields.select({
          name = "currency",
          default_value = "USD",
          options = {
            { label = "USD", value = "USD" },
            { label = "EUR", value = "EUR" },
            { label = "GBP", value = "GBP" },
          },
        }),
      },
    }),
    -- Features array
    crap.fields.array({
      name = "features",
      admin = { label_field = "title", labels = { singular = "Feature", plural = "Features" } },
      fields = {
        crap.fields.text({ name = "title", required = true }),
        crap.fields.checkbox({ name = "included", default_value = true }),
      },
    }),
    crap.fields.upload({ name = "hero_image", relationship = { collection = "media" } }),
  },
  access = {
    read = "access.anyone",
    create = "access.admin_or_director",
    update = "access.admin_or_director",
    delete = "access.admin_only",
  },
})
