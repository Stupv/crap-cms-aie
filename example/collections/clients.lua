crap.collections.define("clients", {
  labels = { singular = "Client", plural = "Clients" },
  timestamps = true,
  admin = {
    use_as_title = "company_name",
    default_sort = "company_name",
    list_searchable_fields = { "company_name", "contact_name", "contact_email" },
  },
  fields = {
    crap.fields.text({ name = "company_name", required = true, admin = { placeholder = "Acme Corp" } }),
    crap.fields.upload({ name = "logo", relationship = { collection = "media" } }),
    crap.fields.text({ name = "website", admin = { placeholder = "https://..." } }),
    crap.fields.date({
      name = "since",
      picker_appearance = "monthOnly",
      admin = { description = "Client since (month/year)" },
    }),
    -- Contact info row
    crap.fields.row({
      name = "contact_row",
      fields = {
        crap.fields.text({ name = "contact_name", admin = { width = "half", placeholder = "Primary contact" } }),
        crap.fields.email({ name = "contact_email", admin = { width = "half", placeholder = "contact@client.com" } }),
      },
    }),
    crap.fields.text({ name = "contact_phone", admin = { placeholder = "+1 555 123 4567" } }),
    crap.fields.select({
      name = "industry",
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
    }),
    crap.fields.textarea({
      name = "notes",
      admin = { rows = 4, description = "Internal notes about this client" },
    }),
    -- Reverse relationship: projects for this client
    crap.fields.join({ name = "client_projects", collection = "projects", on = "client" }),
  },
  access = {
    read = "access.authenticated",
    create = "access.admin_or_director",
    update = "access.admin_or_director",
    delete = "access.admin_only",
  },
})
