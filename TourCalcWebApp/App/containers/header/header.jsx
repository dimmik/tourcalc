import React from 'react';
import { Link } from 'react-router-dom';

export default class Header extends React.Component {
    render() {
        return (
            <header>
                <menu>
                    <ul>
                        <li>
                            <Link to="/">Tour Persons</Link>
                        </li>
                        <li>
                            <Link to="/spendings">Tour Spendings</Link>
                        </li>
                        <li>
                            <Link to="/choose">Choose Tour</Link>
                        </li>
                        <li>
                            <Link to="/auth">Auth</Link>
                        </li>
                    </ul>
                </menu>
            </header>
        );
    }
};