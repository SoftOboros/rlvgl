<!--
rlvgl-decomp/README.md - Décodeur/encodeur RLE pour le format de splash rlvgl.
-->

# rlvgl-decomp

Utilitaires de format d'image compressée de base pour rlvgl.

Cette caisse fournit un format compact d'encodage par plages (RLE) avec une
palette et des codes d'échappement de pixels en ligne, ainsi qu'un encodeur
de base qui construit une palette et émet un flux de répétitions courtes/longues.
Les deux opèrent sur des cadres RGBA et convertissent vers/depuis RGB565 en interne
pour correspondre aux pipelines d'affichage intégrés.

Fonctionnalités :
- Compatible sans std (utilise `alloc`).
- Décodeur pour le format RLE (palette + flux d'octets → RGBA).
- Encodeur de RGBA → palette (RGB565) + flux d'octets utilisant la répétition/dictionnaire.

Le format est un point de départ pour les outils de création afin de convertir des
entrées (par exemple, des cadres PNG/APNG/Lottie) en une représentation compacte
consommable par rlvgl.
