async function call(path, data) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 300000); // 5 minutes

  try {
    const res = await fetch(`http://127.0.0.1:17890${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(data),
      signal: controller.signal
    });
    clearTimeout(timeout);
    return await res.json();
  } catch (e) {
    document.getElementById("output").textContent =
      "Error: " + e.message;
    throw e;
  }
}

document.getElementById("next").onclick = async () => {
  const goal = document.getElementById("goal").value;
  const res = await call("/blender/next_step", { goal });
  document.getElementById("output").textContent = res.step;
};

document.getElementById("doit").onclick = async () => {
  const goal = document.getElementById("goal").value;
  const res = await call("/blender/run_macro", { goal });
  document.getElementById("output").textContent = res.code;
};
