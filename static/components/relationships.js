/**
 * Relationship field "View" links.
 *
 * Shows/hides the link based on the select's current value and updates
 * the href to point to the selected item's edit page.
 */

function initRelationshipViews() {
  document.querySelectorAll('.relationship-field__view-link').forEach(
    /** @param {HTMLAnchorElement} link */ (link) => {
      const selectId = link.getAttribute('data-view-for');
      const collection = link.getAttribute('data-collection');
      if (!selectId || !collection) return;

      const select = /** @type {HTMLSelectElement | null} */ (document.getElementById(selectId));
      if (!select) return;

      /** Update the view link visibility and href */
      const update = () => {
        const val = select.value;
        if (val) {
          const href = '/admin/collections/' + collection + '/' + val;
          link.setAttribute('href', href);
          link.setAttribute('hx-get', href);
          link.style.display = '';
        } else {
          link.style.display = 'none';
        }
      };

      update();
      select.addEventListener('change', update);
    }
  );
}

document.addEventListener('DOMContentLoaded', initRelationshipViews);
document.addEventListener('htmx:afterSettle', initRelationshipViews);
