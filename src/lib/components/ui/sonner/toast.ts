import { toast as sonnerToast } from "svelte-sonner";
import DismissibleToastMessage from "./DismissibleToastMessage.svelte";

type BaseToast = typeof sonnerToast;
type ToastMessage = Parameters<BaseToast>[0];
type ToastData = Parameters<BaseToast>[1];
type TypedToastMethod = (message: ToastMessage, data?: ToastData) => string | number;

let counter = 0;

function makeDismissible(method: TypedToastMethod): TypedToastMethod {
	return (message, data) => {
		if (typeof message !== "string") {
			return method(message, data);
		}

		const id = data?.id ?? `t-${++counter}`;

		return method(DismissibleToastMessage, {
			...data,
			id,
			componentProps: {
				...data?.componentProps,
				message,
				toastId: id,
			},
		});
	};
}

export const toast = Object.assign(
	makeDismissible(sonnerToast),
	{
		message: makeDismissible(sonnerToast.message),
		success: makeDismissible(sonnerToast.success),
		info: makeDismissible(sonnerToast.info),
		warning: makeDismissible(sonnerToast.warning),
		error: makeDismissible(sonnerToast.error),
		loading: sonnerToast.loading,
		promise: sonnerToast.promise,
		custom: sonnerToast.custom,
		dismiss: sonnerToast.dismiss,
		getActiveToasts: sonnerToast.getActiveToasts,
	},
);
