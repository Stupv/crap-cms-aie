crap.collections.define("tags", {
  labels = { singular = "Tag", plural = "Tags" },
  timestamps = true,
  admin = {
    use_as_title = "name",
    default_sort = "name",
    list_searchable_fields = { "name", "slug" },
  },
  fields = {
    crap.fields.text({ name = "name", required = true, admin = { placeholder = "Tag name" } }),
    crap.fields.text({
      name = "slug",
      required = true,
      unique = true,
      hooks = { before_validate = { "hooks.auto_slug" } },
    }),
    crap.fields.select({
      name = "tag_type",
      required = true,
      default_value = "topic",
      options = {
        { label = "Topic", value = "topic" },
        { label = "Technology", value = "technology" },
        { label = "Industry", value = "industry" },
        { label = "Skill", value = "skill" },
      },
    }),
  },
  access = {
    read = "access.anyone",
    create = "access.authenticated",
    update = "access.editor_or_above",
    delete = "access.admin_only",
  },
})
