const form = document.getElementById("form");
let first = false;
function show() {
  document.getElementById("code-row").hidden = !first;
  form.code.required = first;
  form.password.autocomplete = first ? "new-password" : "current-password";
  document.getElementById("title").textContent = first ? "First access" : "Sign in";
  document.getElementById("caption").textContent = first
    ? "Enter the code an admin gave you and choose a password (10 characters or more)."
    : "With the username and password of this server.";
  document.getElementById("password-label").textContent = first ? "New password" : "Password";
  document.getElementById("submit").textContent = first ? "Choose password and sign in" : "Sign in";
  document.getElementById("toggle").textContent = first ? "Already have a password? Sign in" : "First access? Use the code you were given";
}
document.getElementById("toggle").onclick = () => { first = !first; show(); };
form.onsubmit = async (event) => {
  event.preventDefault();
  const body = { name: form.name.value, password: form.password.value };
  if (first) body.code = form.code.value;
  const answer = await fetch(first ? "/auth/first-access" : "/auth/sign-in", {
    method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify(body), credentials: "same-origin",
  });
  if (answer.ok) { location.replace("/"); return; }
  const said = await answer.json().catch(() => ({}));
  document.getElementById("error").textContent = said.error || "Could not sign in.";
};
