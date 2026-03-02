crap.collections.define("media", {
  labels = { singular = "Media", plural = "Media" },
  timestamps = true,
  upload = {
    mime_types = { "image/*", "application/pdf", "video/*", "audio/*" },
    max_file_size = "50MB",
    image_sizes = {
      { name = "thumbnail", width = 300, height = 300, fit = "cover" },
      { name = "card", width = 640, height = 480, fit = "cover" },
      { name = "hero", width = 1920, height = 1080, fit = "cover" },
      { name = "og", width = 1200, height = 630, fit = "cover" },
    },
    format_options = {
      webp = { quality = 80 },
      avif = { quality = 60, queue = true },
    },
    admin_thumbnail = "thumbnail",
  },
  admin = {
    use_as_title = "alt",
    default_sort = "-created_at",
    list_searchable_fields = { "alt", "caption" },
  },
  fields = {
    crap.fields.text({ name = "alt", required = true, admin = { placeholder = "Descriptive alt text" } }),
    crap.fields.text({ name = "caption", localized = true }),
    crap.fields.text({ name = "credit", admin = { placeholder = "Photo credit / attribution" } }),
  },
  access = {
    read = "access.anyone",
    create = "access.authenticated",
    update = "access.authenticated",
    delete = "access.editor_or_above",
  },
})
