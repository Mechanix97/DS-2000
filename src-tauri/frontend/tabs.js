// Sidebar navigation between the "channels".
//
// Selection is a class rather than an inline display, so the stylesheet owns
// what a shown tab looks like and can animate the change.

const menuItems = document.querySelectorAll('.menu-item');
const tabContents = document.querySelectorAll('.tab-content');

/// Shows one tab and marks its sidebar entry, by the tab's element id.
export function selectTab(tabName) {
  menuItems.forEach(item => {
    item.classList.toggle('active', item.dataset.tab === tabName);
  });
  tabContents.forEach(content => {
    content.classList.toggle('active', content.id === tabName);
  });
}

menuItems.forEach(item => {
  item.addEventListener('click', () => selectTab(item.dataset.tab));
});
