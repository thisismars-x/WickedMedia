# WickedMedia

WickedMedia is primarily a zim file reader, which includes a barebone .zim file disassembler written in Rust, allowing the entire Wikipedia to be downloaded for offline usage. This allows the entire Wikipedia, or selective subject domains to be **stored on your local disk for offline usage**. 


I lovingly, also call this application `AskJennifer`.

## About zim
The OpenZim project is responsible for defining the file format for zim files. An alternative zim file reader called [Kiwix](https://download.kiwix.org/zim/), provides download links to zim files, and updates it sometimes. A single .zim file about ~800mb stores everything in wikipedia related to physics. This includes tens of thousands of html, css, js files, and images(png, gif, video) and audio. Learn more about how the file formats are defined check [here](https://wiki.openzim.org/wiki/ZIM_file_format).

## Installation

The entire project is written in Rust. Check [/HELP](./HELP), to learn how to install the application, and how to install related zim files, if you need more help, but this should be enough,

```bash
# Make sure you have Rust, and the cargo package manager
git clone github.com/thisismars-x/WickedMedia

# You also need to download some .zim files, to generate webpages from
# Go here, https://download.kiwix.org/zim/wikipedia/, and download any file which looks like
# *_en_*_maxi.zim, * can be anything.
# Other zim files like those that end with nopic, and mini also work, but, 
# nopic does not include images, and mini may not include all relevant styling
# Then, make a file called store in the same repo, and copy the .zim folder in it
mkdir store

# You have to point the global var JENNIFER to the base pwd, so that later the application can determine
# where the generated webpages are
export JENNIFER=/home/nuke

# You are all set, now go to the repo, and run cargo install --path .
# This will take some time to download the files our rust program depends on
# This will also disassemble the zim files in store/, won't take long
cargo install --path .

# In terminal, run jennifer, and go to localhost:8999(BY_DEFAULT)
jennifer
```

## Why?

Because so much of everything is online now. Seems risky, to depend on the reliability of internet at all times.

I decide to **put the internet in a 512gb hard disk, and stay secure with WickedMedia**!

Want to check it out in motion? [Here!](./base/usage.mp4)

## License

[MIT](https://choosealicense.com/licenses/mit/)
