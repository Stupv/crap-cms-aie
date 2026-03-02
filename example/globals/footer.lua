crap.globals.define("footer", {
  labels = { singular = "Footer" },
  fields = {
    crap.fields.text({
      name = "copyright_text",
      localized = true,
      default_value = "Meridian Studio. All rights reserved.",
    }),
    crap.fields.checkbox({ name = "show_social_links", default_value = true }),
    crap.fields.array({
      name = "partner_logos",
      admin = { label_field = "name", labels = { singular = "Partner", plural = "Partners" } },
      fields = {
        crap.fields.text({ name = "name", required = true }),
        crap.fields.upload({ name = "logo", relationship = { collection = "media" } }),
        crap.fields.text({ name = "url", admin = { placeholder = "https://..." } }),
      },
    }),
  },
  access = {
    read = "access.anyone",
    update = "access.admin_only",
  },
})
