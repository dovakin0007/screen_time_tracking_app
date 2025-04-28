export class HoursAndMinutes {
  hours: number;
  minutes: number;

  constructor(totalMins?: number | null) {
    totalMins = totalMins ?? 0;
    this.hours = Math.floor(totalMins / 60);
    this.minutes = totalMins % 60;
  }

  validateTime() {
    return this.hours * 60 + this.minutes >= 15;
  }
}

export const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), max);

export const getAppIconColor = (appName: string) => {
  const colors = [
    "#4F46E5",
    "#0EA5E9",
    "#10B981",
    "#F59E0B",
    "#EF4444",
    "#8B5CF6",
  ];
  let hash = 0;
  for (let i = 0; i < appName.length; i++) {
    hash = appName.charCodeAt(i) + ((hash << 5) - hash);
  }
  return colors[Math.abs(hash) % colors.length];
};
