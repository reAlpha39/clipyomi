// The tooltip window's entry point. Deliberately tiny and deliberately not
// importing anything from `main.ts`: this page runs in a second webview, and
// pulling in the app entry would start a second clipboard handshake and a
// second set of event listeners against the same backend.
const tooltip = document.querySelector<HTMLElement>('#tooltip')!;

// Task 4 replaces this with the real renderer. For now it proves the page
// loads and the second entry point is wired.
tooltip.textContent = 'tooltip';
