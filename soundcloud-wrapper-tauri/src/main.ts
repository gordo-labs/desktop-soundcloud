const main = () => {
  const container = document.querySelector(".container");
  if (!container) return;

  const meta = document.createElement("p");
  meta.className = "meta";
  meta.textContent = "Proyecto base listo para continuar con la integración de SoundCloud.";

  container.appendChild(meta);
};

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", main);
} else {
  main();
}
