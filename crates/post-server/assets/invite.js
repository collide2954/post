const link = document.getElementById("link");
const button = document.getElementById("copy");

if (link && button) {
  link.addEventListener("focus", () => link.select());
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(link.value);
    } catch {
      link.select();
      document.execCommand("copy");
    }
    button.textContent = button.dataset.done;
  });
}
