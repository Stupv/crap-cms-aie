crap.collections.define("categories", {
  labels = { singular = "Category", plural = "Categories" },
  timestamps = true,
  admin = {
    use_as_title = "title",
    default_sort = "title",
    list_searchable_fields = { "title", "slug" },
  },
  fields = {
    crap.fields.text({
      name = "title",
      required = true,
      hooks = { before_validate = { "hooks.trim_title" } },
      admin = { placeholder = "Category name" },
    }),
    crap.fields.text({
      name = "slug",
      required = true,
      unique = true,
      hooks = { before_validate = { "hooks.auto_slug" } },
    }),
    crap.fields.textarea({ name = "description", admin = { rows = 2 } }),
    crap.fields.relationship({
      name = "parent",
      relationship = { collection = "categories" },
      admin = { description = "Parent category for nesting" },
    }),
    crap.fields.text({ name = "color", admin = { placeholder = "#3b82f6" } }),
  },
  access = {
    read = "access.anyone",
    create = "access.editor_or_above",
    update = "access.editor_or_above",
    delete = "access.admin_only",
  },
})
