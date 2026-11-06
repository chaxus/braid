import { expect, open, test } from "nsh/test";

test("loads demo profile then connects on mock plane", async () => {
  const app = await open(".");
  expect(app.texts()).toContain("Disconnected");
  expect(app.texts()).toContain("Braid");
  expect(
    app.texts().some((t: string) => t.includes("mock plane")),
  ).toBe(true);
  expect(app.texts()).toContain("No profile · — · —");
  await app.press("Demo profile");
  expect(app.texts().some((t: string) => t.includes("sing-box config ready"))).toBe(
    true,
  );
  expect(app.texts().some((t: string) => t.startsWith("Demo node"))).toBe(true);
  await app.press("Connect");
  expect(app.texts()).toContain("Connected");
  expect(
    app.texts().some((t: string) => t.includes("203.0.113.10")),
  ).toBe(true);
  await app.press("Disconnect");
  expect(app.texts()).toContain("Disconnected");
});

test("shows path failure copy", async () => {
  const app = await open(".");
  await app.press("Simulate path failure");
  expect(app.texts().some((t: string) => t.includes("Path to the server"))).toBe(
    true,
  );
  expect(app.texts()).toContain("Failed");
});

test("run diagnostics shows mock copy", async () => {
  const app = await open(".");
  expect(app.texts()).toContain("Diagnose");
  await app.press("Diagnose");
  expect(
    app.texts().some((t: string) => t.includes("Local OK · path OK (mock)")),
  ).toBe(true);
});
