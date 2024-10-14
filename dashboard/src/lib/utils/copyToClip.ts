export async function copyToClip(data: string) {
    await navigator?.clipboard?.writeText(data);
}