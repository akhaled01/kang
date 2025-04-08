<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Document</title>
</head>
<body>
    <?php
        echo "Hello World";
        for ($i = 0; $i < 10; $i++) {
            echo "hi";
            echo "<br>";
        }

        echo $_SERVER['REQUEST_METHOD'];
    ?>
</body>
</html>