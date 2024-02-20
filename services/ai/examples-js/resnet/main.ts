const input = document.querySelector("input");
const output = document.querySelector("output");

input!.addEventListener("change", () => {
  if (input!.files && input!.files.length > 0) {
    if (output) {
      output.innerHTML = `<div class="image"><img src="${
        URL.createObjectURL(input!.files[0])
      }" alt="image"></div>`;
    }
  }
});
