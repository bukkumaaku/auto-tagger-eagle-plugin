import { type ProgressStatus } from "naive-ui";

export interface DownloadOptions {
    url: string;
    dest: string;
    name: string;
    filename: string;
    percentage: number;
    processing: boolean;
    status: ProgressStatus;
    errorText: string;
    downloadedBytes: number;
    totalBytes: number;
    realTimeSpeed: string;
}
