<script lang="ts">
    import { tick } from "svelte";
    import type { Component } from "svelte";
    import * as Command from "$lib/components/ui/command/index.js";
    import * as Dialog from "$lib/components/ui/dialog/index.js";
    import {
        editorState,
        registerEditorPopup,
        syncEditorPopupOpenState,
    } from "$lib/state/editor.svelte";
    import {
        transformationActions,
        type TransformationActionDefinition,
        type TransformationActionId,
    } from "$lib/transformations/actions";
    import { languages } from "$lib/editor/config/supportedLanguages";
    import Zap from "~icons/lucide/zap";

    const languageIconMap = new Map(languages.map((l) => [l.value, l.icon]));

    function getActionIcon(action: TransformationActionDefinition): Component {
        return languageIconMap.get(action.fileTypes[0]) ?? Zap;
    }

    let {
        executeAction,
    }: {
        executeAction: (actionId: TransformationActionId) => Promise<boolean>;
    } = $props();

    let open = $state(false);
    let query = $state("");
    let inputRef = $state<HTMLInputElement | null>(null);

    const suggestedActions = $derived.by(() => {
        return transformationActions.filter((action) => action.fileTypes.includes(editorState.fileType));
    });

    const otherActions = $derived.by(() => {
        return transformationActions.filter((action) => !action.fileTypes.includes(editorState.fileType));
    });

    async function focusInput(): Promise<void> {
        await tick();
        inputRef?.focus();
    }

    async function runAction(actionId: TransformationActionId): Promise<void> {
        if (editorState.loader.visible) {
            return;
        }

        open = false;
        await executeAction(actionId);
    }

    $effect(() => {
        syncEditorPopupOpenState("transformations", open);
    });

    $effect(() => {
        if (!open) {
            query = "";
            return;
        }

        query = "";
        void focusInput();
    });

    $effect(() => {
        return registerEditorPopup("transformations", {
            open: (request) => {
                if (request.id !== "transformations") return;
                open = true;
            },
            close: () => {
                open = false;
            },
        });
    });
</script>

<Dialog.Root bind:open>
    <Dialog.Content
        class="p-0 sm:max-w-[40rem] gap-0"
        showCloseButton={false}
        onOpenAutoFocus={(event) => {
            event.preventDefault();
            void focusInput();
        }}
        onCloseAutoFocus={(event) => {
            event.preventDefault();
            editorState.activeView?.focus();
        }}
    >
        <span class="sr-only">Transformations</span>
        <div class="m-px overflow-hidden rounded-[calc(var(--radius-lg)-1px)]">
            <Command.Root>
                <Command.Input
                    bind:ref={inputRef}
                    bind:value={query}
                    placeholder="Search transformations..."
                />

                <Command.List class="max-h-[22rem]">
                    <Command.Empty>No transformations found.</Command.Empty>

                    {#if suggestedActions.length > 0}
                        <Command.Group heading="Suggested">
                            {#each suggestedActions as action (action.id)}
                                {@const Icon = getActionIcon(action)}
                                <Command.Item
                                    value={action.title}
                                    keywords={action.keywords}
                                    disabled={editorState.loader.visible}
                                    class="items-start py-2"
                                    onSelect={() => {
                                        void runAction(action.id);
                                    }}
                                >
                                    <Icon class="mt-0.5 shrink-0" />
                                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                                        <span class="truncate">{action.title}</span>
                                        <span class="text-xs text-muted-foreground">
                                            {action.description}
                                        </span>
                                    </div>
                                </Command.Item>
                            {/each}
                        </Command.Group>
                    {/if}

                    {#if suggestedActions.length > 0 && otherActions.length > 0}
                        <Command.Separator />
                    {/if}

                    {#if otherActions.length > 0}
                        <Command.Group heading="All Transformations">
                            {#each otherActions as action (action.id)}
                                {@const Icon = getActionIcon(action)}
                                <Command.Item
                                    value={action.title}
                                    keywords={action.keywords}
                                    disabled={editorState.loader.visible}
                                    class="items-start py-2"
                                    onSelect={() => {
                                        void runAction(action.id);
                                    }}
                                >
                                    <Icon class="mt-0.5 shrink-0" />
                                    <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                                        <span class="truncate">{action.title}</span>
                                        <span class="text-xs text-muted-foreground">
                                            {action.description}
                                        </span>
                                    </div>
                                </Command.Item>
                            {/each}
                        </Command.Group>
                    {/if}
                </Command.List>
            </Command.Root>
        </div>
    </Dialog.Content>
</Dialog.Root>