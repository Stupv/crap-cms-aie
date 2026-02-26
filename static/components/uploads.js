/**
 * Upload field preview behavior.
 *
 * Updates preview image and file info when the user selects a different
 * upload from the dropdown.
 */

function initUploadPreviews() {
  document.querySelectorAll('[data-upload-field]').forEach(
    /** @param {HTMLElement} wrapper */ (wrapper) => {
      const select = /** @type {HTMLSelectElement | null} */ (wrapper.querySelector('[data-upload-select]'));
      if (!select) return;
      // Skip if already initialized
      if (select.dataset.previewInit) return;
      select.dataset.previewInit = '1';

      const preview = wrapper.querySelector('.upload-field__preview');
      const info = wrapper.querySelector('.upload-field__info');

      select.addEventListener('change', () => {
        const option = select.options[select.selectedIndex];
        if (!option || !option.value) {
          if (preview) preview.style.display = 'none';
          if (info) info.style.display = 'none';
          return;
        }

        const thumbnail = option.getAttribute('data-thumbnail');
        const filename = option.getAttribute('data-filename');
        const isImage = option.getAttribute('data-is-image') === 'true';

        // Update preview
        if (preview) {
          if (thumbnail && isImage) {
            preview.innerHTML = '<img src="' + thumbnail + '" alt="Preview" />';
            preview.style.display = '';
          } else {
            preview.style.display = 'none';
          }
        }

        // Update info
        if (info) {
          if (filename) {
            info.innerHTML =
              '<span class="material-symbols-outlined" style="font-size: 16px;">description</span>' +
              '<span class="upload-field__filename">' + filename + '</span>';
            info.style.display = '';
          } else {
            info.style.display = 'none';
          }
        }
      });
    }
  );
}

document.addEventListener('DOMContentLoaded', initUploadPreviews);
document.addEventListener('htmx:afterSettle', initUploadPreviews);
