export const genKey = (i: number) => {
    let res = '';

    const target = i || 8;
    for (let i = 0; i < target; i += 1) {
        let nextNumber = 60;
        while ((nextNumber > 57 && nextNumber < 65) || (nextNumber > 90 && nextNumber < 97)) {
            nextNumber = Math.floor(Math.random() * 74) + 48;
        }
        res = res.concat(String.fromCharCode(nextNumber));
    }

    return res;
}