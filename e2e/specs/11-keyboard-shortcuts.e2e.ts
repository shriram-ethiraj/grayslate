import { $, browser, expect } from "@wdio/globals";

describe("keyboard shortcuts help", () => {
  it("opens from Help and searches all shortcut sections", async () => {
    const helpMenu = await $("[data-testid='app-help-menu']");
    await helpMenu.waitForDisplayed();
    await helpMenu.click();

    const shortcutsItem = await $(
      "[data-testid='help-keyboard-shortcuts']",
    );
    await shortcutsItem.waitForDisplayed();
    await shortcutsItem.click();

    const dialog = await $("[data-testid='keyboard-shortcuts-dialog']");
    await dialog.waitForDisplayed();
    const dialogText = await dialog.getText();
    expect(dialogText).toContain("General");
    expect(dialogText).toContain("CSV Table");
    expect(dialogText).toContain("New Slate");
    expect(dialogText).toContain("Edit Focused Cell");

    const search = await $("[data-testid='keyboard-shortcuts-search']");
    await expect(search).toBeFocused();

    await search.setValue("word wrap");
    const actionSearchText = await dialog.getText();
    expect(actionSearchText).toContain("Toggle Word Wrap");
    expect(actionSearchText).not.toContain("New Slate");

    await search.setValue("Ctrl+N");
    const platformKeySearchText = await dialog.getText();
    expect(platformKeySearchText).toContain("New Slate");
    expect(platformKeySearchText).toContain("Ctrl+N");

    await search.setValue("F2");
    const functionKeySearchText = await dialog.getText();
    expect(functionKeySearchText).toContain("Edit Focused Cell");
    expect(functionKeySearchText).toContain("F2");

    await search.setValue("not-a-real-shortcut");
    expect(await dialog.getText()).toContain("No shortcuts found.");

    await browser.keys("Escape");
    await dialog.waitForDisplayed({ reverse: true });
  });
});
